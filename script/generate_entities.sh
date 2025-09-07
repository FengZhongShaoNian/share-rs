#!/bin/bash

has_command(){
  command -v "$1" &> /dev/null
}

if ! has_commmand 'sea-orm-cli'; then
  cargo install sea-orm-cli
fi

project_dir=$(cd "$(dirname "$0")" && cd .. && pwd)

# Create database file
cargo test migrator::test_create_sqlite_database_file

# Generate entity files of database `share_rs_db` to `src/entities`
sea-orm-cli generate entity \
    -u "sqlite:${project_dir}/target/debug/data.db?mode=rwc" \
    -o "${project_dir}/src/backend/entities"
