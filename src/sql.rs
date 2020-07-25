use log::{error, /*warn, info,*/ debug, trace, /*log, Level*/};
use mysql::params::Params;
use mysql::Pool;
use mysql::PooledConn;
use mysql::prelude::FromRow;
use mysql::prelude::Queryable;
use mysql::Statement;
use std::fmt;

use std::sync::RwLock;

use crate::settings::SETTINGS;

lazy_static!
{
    pub static ref MYSQL_CONNECTION_POOL: RwLock<Option<Pool>> = RwLock::new(None);
}

/**
Get a connection to the database.

Internally, it maintains a pool and returns a connection from the pool.
Will log failures at the "error" level.

# Returns
Result indicating whether it was able to get a connection to return.
- `Ok`: A PooledConn object representing your database connection which you can use for queries.
- `Err`: A String describing the error.

# Errors
If there were any errors from the mysql library they will be passed along.

# Panics
Will panic if the function is unable to look into the RwLock containing the connection pool.

# Examples
```no_run
use bitcoin_trend::sql;
let mut db = match sql::connect(){
    Ok(d) => d,
    Err(e) => {panic!("Database error: {}",e);}
};
```
*/
pub fn connect() -> Result<PooledConn, String>
{
    //If the connection pool hasn't been set up, do that now.
    let mut pool_opt = MYSQL_CONNECTION_POOL.write().unwrap();
    let pool = match &*pool_opt {
        Some(p) => p,
        None => {
            //create the pool
            let url = format!("mysql://{}:{}@{}:{}/{}", &SETTINGS.mysql.user, &SETTINGS.mysql.password, &SETTINGS.mysql.host, &SETTINGS.mysql.port, &SETTINGS.mysql.db);
            let pool = match Pool::new(url){
                Ok(p) => p,
                Err(e) => {
                    let e_str = format!("Couldn't connect to mysql: {}", e);
                    error!("{}", e_str);
                    return Err(e_str);
                }
            };

            //store the pool in the global
            *pool_opt = Some(pool);

            //return ref to the pool out of the global
            match &*pool_opt {
                Some(p) => p,
                None => {
                    let e_str = String::from("Couldn't save mysql connection pool");
                    error!("{}", e_str);
                    return Err(e_str);
                }
            }
        }
    };

    //get a connection from the pool
    let conn: PooledConn = match pool.get_conn(){
        Ok(c) => c,
        Err(e) => {
            let e_str = format!("Couldn't get mysql connection from pool: {}",e);
            error!("{}", e_str);
            return Err(e_str);
        }
    };

    Ok(conn)
}

/**
Run a SQL Query where you are expecting to get a result set back (e.g. queries starting with SELECT or SHOW).
Will log failures at the "error" level.

# Parameters
- `conn`: Database connection you got from sql::connect
- `query`: The query string. Can contain parameter placeholders. The types of the columns it will return must match the types you specified in the tuple for RowReturnType.
- `params`: Tuple containing all your parameters. Must match the number of placeholders. Must have the same number of types in the tuple for ParamsType.
- `purpose`: String describing the purpose of the query, used for log messages.

# Returns
Result indicating whether the query was successful.
- `Ok`: The entire result set as a vector of tuples, each tuple representing a row.
- `Err`: String describing the error.

# Examples
```no_run
use bitcoin_trend::sql;
let (segment_size, begin, end): (u64,u64,u64) = (85500, 1338893400, 1347443400);
let mut db = sql::connect().unwrap();
let query = "SELECT a,b FROM prices WHERE c=?,d=?,e=?,f=?";
let prices = sql::query_select::<(u64,u64,u64,u64),(u64,u32)>(
    &mut db, query, (segment_size, segment_size, begin, end), "getting price data for range")
    .unwrap();
```
*/
pub fn query_select<ParamsType: Into<Params>+fmt::Debug, RowReturnType: FromRow>(conn: &mut PooledConn, query: &str, params: ParamsType, purpose: &str) -> Result<Vec<RowReturnType>,String>
{
    trace!("Preparing SQL Query: {}", query);
    let stmt: Statement = match conn.prep(query){
        Ok(s) => s,
        Err(e) => {
            let e_str = format!("SQL Error preparing query - {}: {} Query: {}", purpose, e, query);
            error!("{}", e_str);
            return Err(e_str);
        }
    };

    let params_str = format!("{:?}",&params);
    debug!("Executing Prepared Query: {} -- Params: {}", query, params_str);

    match conn.exec(&stmt,params){
        Ok(set) => Ok(set),
        Err(e) => {
            let e_str = format!("SQL Error executing query - {}: {} Query: {} -- Params: {}", purpose, e, query, params_str);
            error!("{}", e_str);
            Err(e_str)
        }
    }
}

/**
Run a SQL Query where you are not expecting to get a result set back (e.g. queries starting with INSERT or CREATE).
Will log failures at the "error" level.

# Parameters
- `conn`: Database connection you got from sql::connect
- `query`: The query string. Can contain parameter placeholders.
- `params`: Tuple containing all your parameters. Must match the number of placeholders. Must have the same number of types in the tuple for ParamsType.
- `purpose`: String describing the purpose of the query, used for log messages.

# Returns
Result indicating whether the query was successful.
- `Ok`: 1u8
- `Err`: String describing the error.

# Examples
```no_run
use bitcoin_trend::sql;
let (timestamp, price_cents): (u64,u32) = (2354354, 10000);
let mut db = sql::connect().unwrap();
let ins_query = "INSERT INTO `price_history` SET `when`=?, `price_cents`=?";
sql::query(&mut db, ins_query, (timestamp, price_cents), "adding new data point from Bitstamp to database").unwrap();
```
*/
pub fn query<ParamsType: Into<Params>+fmt::Debug>(conn: &mut PooledConn, query: &str, params: ParamsType, purpose: &str) -> Result<u8,String>
{
    trace!("Preparing SQL Query: {}", query);
    let stmt: Statement = match conn.prep(query){
        Ok(s) => s,
        Err(e) => {
            let e_str = format!("SQL Error preparing query - {}: {} Query: {}", purpose, e, query);
            error!("{}", e_str);
            return Err(e_str);
        }
    };

    let params_str = format!("{:?}",&params);
    debug!("Executing Prepared Query: {} -- Params: {}", query, params_str);

    match conn.exec_drop(&stmt,params){
        Ok(_) => Ok(1),
        Err(e) => {
            let e_str = format!("SQL Error executing query - {}: {} Query: {} -- Params: {}", purpose, e, query, params_str);
            error!("{}", e_str);
            Err(e_str)
        }
    }
}
