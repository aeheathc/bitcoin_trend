use log::{error, warn, info, /*debug,*/ trace, /*log, Level*/};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::thread;
use std::time::Duration;

use crate::sql;

/**
Represents the response we get from the bitstamp API.

Even though all of the data is logically numeric, most of the fields come back
explicity quoted, making them Strings which have to be parsed into numbers separately.
"vwap" is the field containing the price we store.
*/
#[derive(Serialize, Deserialize)]
struct BitstampHourlyResponse {
    high: String,
    last: String,
    timestamp: String,
    bid: String,
    vwap: String,
    volume: String,
    low: String,
    ask: String,
    open: f32
}

/**
Ensures that the database contains the table we will be using.
If we have to create it, also populate it with the historical data from Kaggle.

# Returns
bool indicating whether the initialization was successful.

# Errors
Returns false on problems that are not immediately recoverable such as database errors or file read errors.

# Examples
```no_run
use bitcoin_trend::updater;

//Initialize the DB if necessary, bail if we couldn't
if !updater::db_init() {std::process::exit(1);}
```
*/
pub fn db_init() -> bool
{
    //open DB
    let mut db = match sql::connect(){
        Ok(d) => d,
        Err(_) => {
            error!("Couldn't start database initializer: Couldn't connect to DB");
            return false;
        }
    };

    //If table doesn't exist, create it and populate with base historical data
    let query_exists = "SHOW TABLES LIKE 'price_history'";
    match sql::query_select::<(),String>(&mut db, query_exists, (), "checking for table price_history")
    {
        Err(_) => {
            error!("Updater crashed: couldn't check for history table");
            return false;
        },
        Ok(res) =>{
            if res.is_empty()
            {
                //Create table
                let query_create = "CREATE TABLE `price_history` (`when` BIGINT unsigned NOT NULL, `price_cents` int(11) unsigned NOT NULL, PRIMARY KEY (`when`)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";
                if sql::query(&mut db, query_create, (), "making sure price_history table exists").is_err()
                {
                    error!("Updater crashed during db init: couldn't create history table");
                    return false;
                }

                //Populate
                let csv_file = match File::open("history/bitstamp.csv")
                {
                    Ok(f) => f,
                    Err(_) => {
                        error!("Updater crashed during db init: couldn't open history file");
                        return false;
                    }
                };
                let reader = BufReader::new(csv_file);
                let query_ins = "INSERT INTO `price_history` SET `when`=?,`price_cents`=?";
                for line_res in reader.lines()
                {
                    match line_res {
                        Err(e)=>{
                            warn!("Updater db init failed to read a line from file, skipping: {}", e);
                            continue;
                        },
                        Ok(line)=>{
                            let sep_index = match line.find(',') {None=>{continue;},Some(n)=>n};
                            let timestamp = match line.chars().take(sep_index  ).collect::<String>().parse::<u64>() {Err(_)=>{continue;},Ok(n)=>n};
                            let price     = match line.chars().skip(sep_index+1).collect::<String>().parse::<f32>() {Err(_)=>{continue;},Ok(n)=>n};
                            let price_cents: u32 = (price * 100.0) as u32;
                            
                            if let Err(e) = sql::query(&mut db, query_ins, (timestamp, price_cents), "inserting value from csv")
                            {
                                warn!("Updater db init failed to insert line [{},{}], skipping -- {}", timestamp, price_cents, e);
                            }
                        }
                    }
                }
                info!("Finished populating newly created history table with base data.");
            }
        }
    }

    true
}

/**
Start the database updater loop that will run forever, waiting an hour between each attempt to update.
It is up to the caller to run this in a separate thread, or be blocked indefinitely.

# Errors
On most errors it will simply wait another hour before trying again.
On serious errors likely to happen again every time, it will terminate.
In either case, it will log what went wrong.

# Examples
```no_run
use bitcoin_trend::updater;
use std::thread;
//Keep the DB updated while the app runs
thread::spawn(|| { updater::updater(); });
```
*/
pub fn updater()
{
    let mut first_iter = true;
    loop{
        /* Wait an hour between iterations.
        We have this first_iter guard to start immediately the first time,
        which wouldn't be necessary if we just put the sleep at the end of the loop instead,
        but doing it this way allows using `continue` to abort bad iterations without skipping the sleep.
        */
        if first_iter
        {
            first_iter = false;
        }else{
            thread::sleep(Duration::from_secs(60*60));
        }

        trace!("Iterating hourly update loop");

        //Check that the data isn't already fresh just to make extra sure we're not abusing the Bitstamp API
        match sql::connect(){
            Err(_) => {continue;},
            Ok(mut db) =>
            {
                let check_query = "SELECT `when` FROM `price_history` WHERE `when` = (SELECT MAX(`when`) FROM `price_history`) LIMIT 1";
                match sql::query_select::<(),u64>(&mut db, check_query, (), "checking freshness")
                {
                    Err(_) => {continue;},
                    Ok(res) =>{
                        if res.is_empty()
                        {
                            let latest_ts = res[0];
                            let now = chrono::offset::Utc::now().timestamp();
                            let half_hour_in_seconds = 60*30;
                            if now - (latest_ts as i64) < half_hour_in_seconds
                            {
                                info!("Database is less than a half hour old; will wait till next iteration before calling out to external API.");
                                continue;
                            }
                        }
                    }
                }
            }
        };

        //Call out to the Bitstamp API
        let mut curlobj = curl::easy::Easy::new();
        if let Err(e) = curlobj.url("https://www.bitstamp.net/api/ticker_hour/")
        {
            error!("Updater couldn't parse API URL; Bailing! Reason: {}", e);
            return;
        }
        
        if let Err(e) = curlobj.write_function(
        |data|{
            //Parse the JSON response from the API
            let response = match serde_json::from_slice::<BitstampHourlyResponse>(data)
            {
                Err(e) =>{warn!("Updater couldn't parse JSON from Bitstamp API! Reason: {}",e); return Ok(0);}
                Ok(r) => r,
            };
            let price_cents: u32 = match response.vwap.parse::<f64>(){
                Err(e) => {warn!("Updater couldn't parse price recieved from API: {}",e); return Ok(0);},
                Ok(p) => (p * 100.0) as u32
            };
            let timestamp: u64 = match response.timestamp.parse::<u64>(){
                Err(e) => {warn!("Updater couldn't parse timestamp recieved from API: {}",e); return Ok(0);},
                Ok(p) => p
            };

            //Store the data we got
            let mut db = match sql::connect(){
                Err(e) => {error!("Database updater parsed API value, but couldn't open DB connection! Error: {}",e); return Ok(0);},
                Ok(d) => d,
            };

            let ins_query = "INSERT INTO `price_history` SET `when`=?, `price_cents`=?";
            let _ = sql::query(&mut db, ins_query, (timestamp, price_cents), "adding new data point from Bitstamp to database");

            Ok(data.len())
        }){
            error!("Updater couldn't assign callback to CURL; Bailing! Reason: {}", e);
            return;
        }

        if let Err(e) = curlobj.perform(){
            warn!("API Call to Bitstamp execution failed: {}", e);
        }
    }
}