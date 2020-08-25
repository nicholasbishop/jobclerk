# jobclerk

## Test database

Run postgres in a Docker container:

    tools/run_postgres.py 

The dbctl command can initialize the database as well as clean it
(drop the tables) and add test data:

    cargo run --bin dbctl -- init
    cargo run --bin dbctl -- test
    cargo run --bin dbctl -- clean
