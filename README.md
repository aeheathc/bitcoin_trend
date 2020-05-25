# Bitcoin Trend
Example actix-web app showing a chart of Bitcoin prices over time using a default set of historical data, and keeping itself updated over time using the Bitstamp API.

## Requirements
- Docker

## How to run
- Run `docker-compose up -d` in the project root (the directory containing docker-compose.yml).

After several minutes of compiling and data import, the app will be available at http://localhost:4000 . You can monitor the status of compilation in the container's main stdout log. Data import should take about as long as the compilation did and you can monitor it by connecting to mysql manually (localhost:3306) to watch the table grow. Once you see INFO level log output, especially "Finished populating newly created history table with base data" (also shown in data/log/main.log) the web interface will begin to work.

If you want the app to be on some other port due to conflicts, first edit docker-compose.yml. Find the item 4000:80 and change 4000 to be whatever port you want.

## Caveats
- The spec said to have a default range of the last 24 hours and that input sanity checks include making sure the range isn't so large that the dataset slows down the UI. I interpreted this as intending to allow simpler logic in processing the data. However, since this program is capable of displaying any date range quickly, I made the default range be the entire dataset because it looks better. You can still adjust the slider to very small ranges if you want.
- There will be a gap in the data between 2020-04-22 and when you first start the app in your environment. The kaggle page linked in the spec doesn't seem to have the "feed" mentioned in the spec, only an archive of past data that ends at 2020-04-22. The app does keep itself updated using the Bitstamp API, but that only provides the current prices (updating hourly) and no historical data -- thus, the gap. If you let the app run for 24 hours, then the "past 24 hours" chart will look much better. That said, the app interpolates and extrapolates as necessary, for missing data inside the valid range, so it shouldn't look too bad either way.
- The spec said to use Dockerfiles for each component. I took this to mean making the run process as simple as possible and not requiring very long commands to run. In truth I specified everything in the docker-compose.yml instead of using any Dockerfiles. I don't have a strong opinion on this approach -- I have never used Docker before and this seemed like the easiest way.

## Other decisions
- JSON is used for the AJAX responses because it was the easiest; actix has builtin support for returning JSON.
- Rust was used for the backend because I hadn't used actix before and wanted to learn something new.
- The app uses config files, and there are a few ways of solving the problem of delivering the default config while having the actual file in your .gitignore to allow local config changes to not show up as changes in Git. In this project I chose to not provide the default config as a physical file, and have the program generate the file with default values if it is not present.

## Other things you can do with the code
The commands in this section can be run normally in the project root if you have Rust installed. Otherwise, you can run them inside the container instead. You can get a shell in the container, when the app is running, with `docker exec -it bitcoin_trend_app_1 /bin/bash`

- Run `cargo test` to run the unit tests
- Run `cargo clippy` to run the linter
- Run `cargo doc` to build HTML docs from the "doc comments" found in the source. The docs will be available at `target/doc/bitcoin_trend/index.html`
- If the source file from Kaggle gets updated you can translate/reduce it to the format compatible with this program by using `data/history/filter_bitstamp.sh` and use the output file to replace the existing `data/history/bitstamp.csv`. The output is only around 1MB, much more portable than the original file.