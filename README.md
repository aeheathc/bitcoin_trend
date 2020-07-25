# Bitcoin Trend
Example actix-web app showing a chart of Bitcoin prices over time using a default set of historical data, and keeping itself updated over time using the Bitstamp API.

## Requirements
- Docker

## How to run
- Run `docker-compose up -d` in the project root (the directory containing docker-compose.yml).

After several minutes of compiling and data import, the app will be available at http://localhost:4000 . You can monitor the status of compilation in the container's main stdout log. Data import should take about as long as the compilation did and you can monitor it by connecting to mysql manually (localhost:3306) to watch the table grow. Once you see INFO level log output, especially "Finished populating newly created history table with base data" (also shown in data/log/main.log) the web interface will begin to work.

If you want the app to be on some other port due to conflicts, first edit docker-compose.yml. Find the item 4000:80 and change 4000 to be whatever port you want.

## Caveats
- There will be a gap in the data between "the end date of the historical data at the time it was pulled from kaggle" and "when you first start the app in your environment". The app does keep itself updated using the Bitstamp API, but that only provides the current prices (updating hourly) and no historical data -- thus, the gap. If you let the app run for 24 hours, then the "past 24 hours" chart will look much better. That said, the app interpolates and extrapolates as necessary, for missing data inside the valid range, so it shouldn't look too bad either way.
- The app uses config files, and there are a few ways of solving the problem of delivering the default config while having the actual file in your .gitignore to allow local config changes to not show up as changes in Git. In this project I chose to not provide the default config as a physical file, and have the program generate the file with default values if it is not present.

## Other things you can do with the code
The commands in this section can be run normally in the project root if you have Rust installed. Otherwise, you can run them inside the container instead. You can get a shell in the container, when the app is running, with `docker exec -it bitcoin_trend_app_1 /bin/bash`

- Run `cargo test` to run the unit tests
- Run `cargo clippy` to run the linter
- Run `cargo doc` to build HTML docs from the "doc comments" found in the source. The docs will be available at `target/doc/bitcoin_trend/index.html`
- If the source file from Kaggle gets updated you can translate/reduce it to the format compatible with this program by using `data/history/filter_bitstamp.sh` and use the output file to replace the existing `data/history/bitstamp.csv`. The output is only around 1MB, much more portable than the original file.