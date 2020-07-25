use actix_web::{web, App, HttpServer};
use log::{/*error, warn,*/ info, /*debug, trace, log, Level*/};
use std::thread;

use bitcoin_trend::pages;
use bitcoin_trend::settings;
use settings::SETTINGS;
use bitcoin_trend::updater;

/**
Main entry point.

This first ensures the database is in a good state, then starts the ongoing threads for
the database updater and the HTTP listener.
Note that before execution even gets here, the configuration and logger have already been set up by
the lazy_static code in the settings module.

# Returns
Result, but only when actix-web fails to bind to the port we want to use for HTTP.

# Panics
Will panic if something went wrong with ensuring correct database state on startup.
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
            .route("/", web::get().to(pages::index))                            // request for root: this delivers the main app page that users see
            .route("/api/prices/{begin}/{end}", web::get().to(pages::api))     // ajax calls get recieved here, we split part of the path into args
            .service(actix_files::Files::new("/static", "static").disable_content_disposition())   // serve static files from given dir
            .default_service(web::route().to(pages::notfound))                  // where to go when nothing else matches
    })
    .bind(&SETTINGS.startup.listen_addr)?
    .run()
    .await
}

