extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde;

use actix_web::{web, App, HttpResponse, HttpServer, http::header, http::StatusCode};
use actix_http::ResponseBuilder;
use log::{/*error, warn,*/ info, /*debug, trace, log, Level*/};
use std::cmp;
use std::thread;

mod settings;
use settings::SETTINGS;
mod sql;
mod updater;

/**
Main entry point.

This first ensures the database is in a good state, then starts the ongoing threads for
the database updater and the HTTP listener.
Note that before execution even gets here, the configuration and logger have already been set up by
the lazy_static code in the settings module.

# Returns
Result, but only when actix-web fails to bind to the port we want to use for HTTP.

# Panics
Will panic if something went wrong ensuring correct database state on startup.
*/
#[actix_rt::main]
async fn main() -> std::io::Result<()>
{
    info!("Starting bitcoin_trend on {}", &SETTINGS.startup.listen_addr);

    //Initialize the DB if necessary, bail if we couldn't
    if !updater::db_init() {panic!("Couldn't initialize database, see log for details.");}
    
    //Keep the DB updated while the app runs
    thread::spawn(|| { updater::updater(); });

    //Start the HTTP server
    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))                            // request for root: this delivers the main app page that users see
            .route("/api/prices/{begin}/{end}", web::get().to(api))     // ajax calls get recieved here, we split part of the path into args
            .service(actix_files::Files::new("/static", "static").disable_content_disposition())   // serve static files from given dir
            .default_service(web::route().to(notfound))                  // where to go when nothing else matches
    })
    .bind(&SETTINGS.startup.listen_addr)?
    .run()
    .await
}

/**
Responds to requests for the main page at the domain root.

# Returns
HttpResponse containing the main page, which is the same every time -- everything dynamic is in the frontend code.
*/
async fn index() -> HttpResponse
{
    let body = "<div id='price_chart_container'><canvas id='price_chart'></canvas></div><br/><div id='slider'></div><br/><span id='begin'></span> - <span id='end'></span><img src='static/loading.gif' id='spinner'/>";
    let head = "<script>$( function() {chart_init();});</script>";

    let html = html_construct("Home - Bitcoin Trend", head, body);

    ResponseBuilder::new(StatusCode::OK)
        .set_header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html)
}

/**
Responds to requests for the api endpoint "prices"

# Parameters
- `range`: actix-generated tuple containing the captured parameters "begin" and "end"

# Returns
HttpResponse containing (if successful) JSON with the requested data.

# Errors
The HttpResponse can also indicate failure, which happens when anything goes wrong like
invalid input or a database error. In this case the body will still be JSON, but it will
only contain a string describing the error.
*/
async fn api(range: web::Path<(u64, u64)>) -> HttpResponse
{
    let mut db = match sql::connect(){
        Ok(d) => d,
        Err(e) => {
            let e_str = format!("Database error: {}",e);
            return ResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR)
                .set_header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .json(e_str);
        }
    };
    let begin = range.0;
    let end = range.1;
    let segment_size = cmp::max((end - begin) / 100, 1);

    if end < begin {
        return ResponseBuilder::new(StatusCode::BAD_REQUEST)
        .set_header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .json("begin (frst value) must be <= end (second value)");
    }

    /* Get prices for the range specified.
    - If there isn't a data point exactly on the given begin/end points, use the closest value outside the range. (COALESCE with subquery)
      - Support this by including virtual data points at the beginning and end of time that match the closest values (FROM UNION)
    - Resample the data over 100 segments so we can return any range in the same amount of time. (GROUP BY `when` DIV segment_size)
    */
    let range_query = "
SELECT 
    `segment_num` * ? AS `when`,
    `avg_price_cents` AS avg_price_cents
FROM(
	SELECT
		FLOOR(`when` DIV ?) AS segment_num,
		FLOOR(AVG(`price_cents`))  AS avg_price_cents
	FROM(
		SELECT * FROM `price_history`
		UNION SELECT 0,439
		UNION SELECT
			~0,
			(
				SELECT `price_cents`
				FROM `price_history`
				WHERE `when`=(SELECT MAX(`when`) FROM `price_history`)
			)
	) AS prices
	WHERE `when` >= COALESCE((SELECT MAX(`when`) FROM `price_history` WHERE `when` <= ?), 0)
		AND `when` <= COALESCE((SELECT MIN(`when`) FROM `price_history` WHERE `when` >= ?), ~0)
	GROUP BY `segment_num`
) AS segmented_averages
ORDER BY `when`
    ".replace("\n"," ").replace("\r"," ");

    let prices = match sql::query_select::<(u64,u64,u64,u64),(u64,u32)>(&mut db, &range_query, (segment_size, segment_size, begin, end), "getting price data for range")
    {
        Err(e) => {
            let e_str = format!("Database error: {}",e);
            return ResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR)
                .set_header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .json(e_str);
        },
        Ok(r) => r
    };

    ResponseBuilder::new(StatusCode::OK)
        .set_header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .json(prices)
}

/**
Responds to requests that don't match anything we have.

# Returns
HttpResponse indicating HTTP 404 Not Found.
*/
async fn notfound() -> HttpResponse
{
    let html = html_construct("Not Found - Bitcoin Trend", "", "<h1>Not Found</h1><a href='/'>Return to Home</a>");

    ResponseBuilder::new(StatusCode::NOT_FOUND)
        .set_header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html)
}

/**
Generates a complete HTML document given the elements that change between pages.
This is where we define all the external static resources included in every page, and other HTML boilerplate.

# Parameters
- `title`: The contents of the title tag, which browsers tend to display in their title bar
- `head_extra`: HTML content to be included in the root of the head tag, intended for page-specific styles/scripts
- `body`: contents of the body tag

# Returns
String containing the HTML document.None

# Examples
```
let html = html_construct("Not Found", "", "<h1>Not Found</h1><a href='/'>Return to Home</a>");
assert_eq!(html[..15],"<!DOCTYPE html>");
```
*/
fn html_construct(title: &str, head_extra: &str, body: &str) -> String
{
    format!("<!DOCTYPE html>
<html>
 <head>
  <meta charset='utf-8'/>
  <meta http-equiv='X-UA-Compatible' content='IE=edge'/>
  <meta name='viewport' content='height=device-height, width=device-width, initial-scale=1'/>
  <link rel='shortcut icon' href='static/favicon.ico'/>
  <script src='static/lib/jquery.min.js'></script>
  <script src='static/lib/jquery-ui/jquery-ui.min.js'></script>
  <script src='static/lib/moment-with-locales.js'></script>
  <link rel='stylesheet' href='static/lib/jquery-ui/jquery-ui.min.css'/>
  <script src='static/lib/chartjs/Chart.min.js'></script>
  <link rel='stylesheet' href='static/lib/chartjs/Chart.min.css'/>
  <script src='static/main.js'></script>
  <link rel='stylesheet' href='static/main.css'/>
  {}
  <title>{}</title>
 </head>
 <body>
 {}
 </body>
</html>",
    head_extra, title, body)
}


/*
Test those functions which weren't able to have good tests as part of their
example usage in the docs, but are still possible to unit-test
*/
#[cfg(test)]
mod tests
{
    use super::*;

	// html_construct
	#[test]
	fn gen_page()
	{
        let html = html_construct("Not Found", "", "<h1>Not Found</h1><a href='/'>Return to Home</a>");
        assert_eq!(&html[..15],"<!DOCTYPE html>");
    }

}