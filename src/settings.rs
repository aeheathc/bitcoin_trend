use clap::{Arg, App};
use config::{ConfigError, Config, File};
use log::{error/*, warn, info, debug, trace, log, Level*/};
use std::env;
use std::fs;
use std::path::Path;

/**
The portion of the config needed immediately, before we can even do so much as display an error over HTTP.
*/
#[derive(Deserialize)]
pub struct Startup
{
    pub working_dir: String,
    pub listen_addr: String
}

/**
The portion of the config needed for mysql database connections.
*/
#[derive(Deserialize)]
pub struct Mysql
{
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub db: String
}

/**
The main type storing all the configuration data.
*/
#[derive(Deserialize)]
pub struct Settings
{
    pub startup: Startup,
    pub mysql: Mysql
}

impl Settings
{
    /**
    Generates a TOML format config file containing the values set in this struct.

    # Examples
    ```
    use bitcoin_trend::settings::*;
    let def_settings: Settings = Settings{
        startup: Startup{
            working_dir: String::from("data"),
            listen_addr: String::from("0.0.0.0:80")
        },
        mysql: Mysql{
            host: String::from("db_host"),
            port: 3306,
            user: String::from("root"),
            password: String::from("passw0rd"),
            db: String::from("database_1")
        }
    };

    let default_config_file_contents = def_settings.to_toml();

    assert_eq!(&default_config_file_contents[..30],"[startup]\nworking_dir = \"data\"");
    ```
    */
    pub fn to_toml(&self) -> String
    {
        format!("[startup]\nworking_dir = \"{}\"\nlisten_addr = \"{}\"\n[mysql]\nhost = \"{}\"\nport = {}\nuser = \"{}\"\npassword = \"{}\"\ndb = \"{}\"\n",
            self.startup.working_dir, self.startup.listen_addr, self.mysql.host, self.mysql.port, self.mysql.user, self.mysql.password, self.mysql.db)
    }

    /**
    Load configuration for app and logger.

    - Load app & logger config, merging values from all sources (cmd, env, file, defaults) with appropriate priority
    - Store app config in a lazy_static ref settings::SETTINGS
    - Set the working directory of the app to what is configured, so relative paths work correctly.
    - If either config file is missing, write a new one with default settings.
    - Start up logger.

    # Panics
    This function makes every attempt to recover from minor issues, but any unrecoverable problem will result in a panic.
    After all, the app can't safely do much of anything without the info it returns, and even the logger isn't available until the very end.
    Possible unrecoverables include CWD change error, filesystem errors, and config parse errors.

    # Undefined behavior
    This should only be called once. Additional calls may result in issues with the underlying config and logger libraries.

    */
    fn new() -> Self
    {
        let path_config = "config/config.toml";
        let path_log4rs_config = "config/log4rs.yml";
        let mysql_default_port_str = format!("{}",DEFAULT_SETTINGS.mysql.port);
        //std::env::set_var("RUST_LOG", "my_errors=debug,actix_web=info");
        //std::env::set_var("RUST_BACKTRACE", "1");
        
        //Load command-line arguments. For those unspecified, load environment variables.
        let cmd_matches = App::new("bitcoin_trend")
            .version("dev")
            .about("Simple actix-web app showing bitcoin prices over time")
            .arg(Arg::with_name("working_dir")
                .short("w")
                .long("workingdir")
                .env("BITCOIN_TREND_WORKING_DIR")
                .help("Working directory. Will look here for the folders config,history,logs,static -- particularly the config file in config/config.toml which will be created if it doesn't exist.")
                .default_value(&DEFAULT_SETTINGS.startup.working_dir)
                .takes_value(true))
            .arg(Arg::with_name("listen_addr")
                .short("l")
                .long("listenaddr")
                .env("BITCOIN_TREND_LISTEN_ADDR")
                .help("ip:port to listen on. Use 0.0.0.0 for the ip to listen on all interfaces.")
                .default_value(&DEFAULT_SETTINGS.startup.listen_addr)
                .takes_value(true))
            .arg(Arg::with_name("mysql_host")
                .short("h")
                .long("mysql-host")
                .env("BITCOIN_TREND_MYSQL_HOST")
                .help("Open the mysql connection to this host")
                .default_value(&DEFAULT_SETTINGS.mysql.host)
                .takes_value(true))
            .arg(Arg::with_name("mysql_port")
                .short("o")
                .long("mysql-port")
                .env("BITCOIN_TREND_MYSQL_PORT")
                .help("Open the mysql connection to this port number")
                .default_value(&mysql_default_port_str)
                .takes_value(true))
            .arg(Arg::with_name("mysql_user")
                .short("u")
                .long("mysql-user")
                .env("BITCOIN_TREND_MYSQL_USER")
                .help("Username for the mysql connection")
                .default_value(&DEFAULT_SETTINGS.mysql.user)
                .takes_value(true))
            .arg(Arg::with_name("mysql_password")
                .short("p")
                .long("mysql-password")
                .env("BITCOIN_TREND_MYSQL_PASSWORD")
                .help("Password for the mysql connection")
                .default_value(&DEFAULT_SETTINGS.mysql.password)
                .takes_value(true))
            .arg(Arg::with_name("mysql_db")
                .short("d")
                .long("mysql-db")
                .env("BITCOIN_TREND_MYSQL_DB")
                .help("Database name for the mysql connection")
                .default_value(&DEFAULT_SETTINGS.mysql.db)
                .takes_value(true))
            .get_matches();
    
        //set cwd
        let working_dir = cmd_matches.value_of("working_dir").expect("Couldn't determine target working dir");
        env::set_current_dir(Path::new(working_dir)).expect("Couldn't set cwd");

        //attempt to load config file
        let mut file_config = Config::new();
        if let Err(ce) = file_config.merge(File::with_name(&path_config))
        {
            match ce //determine reason for failure
            {
                ConfigError::Frozen => panic!("Couldn't load config because it was already frozen/deserialized"),
                ConfigError::NotFound(prop) => panic!("Couldn't load config because the following thing was 'not found': {}",prop),
                ConfigError::PathParse(ek) => panic!("Couldn't load config because the 'path could not be parsed' due to the following: {}", ek.description()),
                ConfigError::FileParse{uri: _, cause: _} => {panic!("Couldn't load config because of a parser failure.")},
                ConfigError::Type{origin:_,unexpected:_,expected:_,key:_} => panic!("Couldn't load config because of a type conversion issue"),
                ConfigError::Message(e_str) => panic!("Couldn't load config because of the following: {}", e_str),
                ConfigError::Foreign(_) =>{
                    //looks like the file is missing, attempt to write new file with defaults then load it. If this also fails then bail
                    if let Err(e) = fs::write(String::from(path_config), DEFAULT_SETTINGS.to_toml()){
                        panic!("Couldn't read main config file or write default main config file: {}", e);
                    }
                    file_config.merge(File::with_name(&path_config)).expect("Couldn't load newly written default main config file.");
                }
            }
        }

        //command line arguments, if given, override what is in the config file
        let set_e = "Couldn't override config setting";
        if cmd_matches.occurrences_of("working_dir"   ) > 0 {file_config.set("startup.working_dir", cmd_matches.value_of("working_dir"   )).expect(set_e);}
        if cmd_matches.occurrences_of("listen_addr"   ) > 0 {file_config.set("startup.listen_addr", cmd_matches.value_of("listen_addr"   )).expect(set_e);}
        if cmd_matches.occurrences_of("mysql_host"    ) > 0 {file_config.set("mysql.host",          cmd_matches.value_of("mysql_host"    )).expect(set_e);}
        if cmd_matches.occurrences_of("mysql_port"    ) > 0 {file_config.set("mysql.port",          cmd_matches.value_of("mysql_port"    )).expect(set_e);}
        if cmd_matches.occurrences_of("mysql_user"    ) > 0 {file_config.set("mysql.user",          cmd_matches.value_of("mysql_user"    )).expect(set_e);}
        if cmd_matches.occurrences_of("mysql_password") > 0 {file_config.set("mysql.password",      cmd_matches.value_of("mysql_password")).expect(set_e);}
        if cmd_matches.occurrences_of("mysql_db"      ) > 0 {file_config.set("mysql.db",            cmd_matches.value_of("mysql_db"      )).expect(set_e);}

        //attempt to load logging config
        if let Err(le) = log4rs::init_file(path_log4rs_config, Default::default())
        {
            match le //determine reason for failure
            {
                log4rs::Error::Log4rs(_) =>
                {
                    //looks like the file is missing, attempt to write new file with defaults then load it. If this also fails then bail
                    if let Err(e) = fs::write(String::from(path_log4rs_config), DEFAULT_LOG4RS.to_string()){
                        panic!("Couldn't read log config file or write default log config file: {}", e);
                    }
                    log4rs::init_file(path_log4rs_config, Default::default()).expect("Couldn't load newly written default log config file.");
                },
                _ => {panic!("Couldn't parse log config.");}
            }
        }

        //Export config to Settings struct
        match file_config.try_into()
        {
            Err(_) => {let e = "Couldn't export config."; error!("{}",e); panic!(e);},
            Ok(s) => s
        }
    }
}

lazy_static!
{
    pub static ref SETTINGS: Settings = Settings::new();

    static ref DEFAULT_SETTINGS: Settings = Settings{
        startup: Startup{
            working_dir: String::from("data"),
            listen_addr: String::from("0.0.0.0:80")
        },
        mysql: Mysql{
            host: String::from("db"),
            port: 3306,
            user: String::from("root"),
            password: String::from("j23f24hgf359bgfu4gf4o0i34nf0oi4g"),
            db: String::from("bitcoin_trend")
        }
    };

    static ref DEFAULT_LOG4RS: String = String::from("refresh_rate: 60 seconds
appenders:
  stdout:
    kind: console
    target: stdout
  stderr:
    kind: console
    target: stderr
  main:
    kind: file
    path: \"log/main.log\"
    encoder:
      pattern: \"{d} [{P}:{I}] {l} - {m}{n}\"
  requestlog:
    kind: file
    path: \"log/requests.log\"
    encoder:
      pattern: \"{d} [{P}:{I}] - {m}{n}\"
root:
  level: info
  appenders:
    - main
    - stdout
loggers:
  requests:
    level: info
    appenders:
      - requestlog
    additive: false");
}

/*
Test those functions which weren't able to have good tests as part of their
example usage in the docs, but are still possible to unit-test
*/
#[cfg(test)]
mod tests
{
    use super::*;

	// settings::Settings::new()
	#[test]
	fn config_load()
	{
        //if this function panics, that is what will make the test fail, so no assert is needed.
        let _config = Settings::new();
    }

    // settings::Settings.to_toml()
    #[test]
    fn file_gen()
    {
        let def_settings: Settings = Settings{
            startup: Startup{
                working_dir: String::from("data"),
                listen_addr: String::from("0.0.0.0:80")
            },
            mysql: Mysql{
                host: String::from("db_host"),
                port: 3306,
                user: String::from("root"),
                password: String::from("passw0rd"),
                db: String::from("database_1")
            }
        };

        let default_config_file_contents = def_settings.to_toml();

        assert_eq!(&default_config_file_contents[..30],"[startup]\nworking_dir = \"data\"");
    }
}