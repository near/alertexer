# Alertexer

An indexer toolset to empower the Alerts Feature in DevConsole.

[**Please, refer to the Docs to find the Concept descriptions for main parts of the toolset**](./docs)

## Structure

This project is using `workspace` feature of Cargo.

### Crates

- [`alert-rules`](./alert-rules) crate provides the `AlertRule` type for usage in other crates
- [`shared`](./shared) crate holds the common `clap` structs for every indexer in the workspace. Also, it includes shared types and utils.
- [`storage`](./storage) crate provides the functions to work with Redis that are common for all indexers in the workspace

### Indexers

- [`alertexer`](./alertexer) an indexer to watch for `AlertRules`
- *deprecated* [`tx-alertexer`](./tx-alertexer) (excluded from the workspace) an indexer that watches for entire transaction and collects all the stuff related to the transaction.

### External-ish stuff

Closely related to the project but excluded from the workspace for different reasons crates.

- [`queue-handler-alertexer`](./queue-handler-alertexer) is an AWS lambda function (Rust-lang) that listens to the events in main AWS SQS queue for alertexer. Interacts with the DevConsole DB to get data about the `AlertRule` and stores info about triggered events, passed the triggered event to the relevant queue based on the delivery channel.
- [`webhook-queue-handler`](./webhook-queue-handler) is an AWS lambda function (Rust-lang) that listens to the events in the dedicated AWS SQS queue for webhooks. Interacts with the DB to store an information about the webhook is sent and what was the response (in order to simplify the webhook debugging)
- [`telegram-queue-handler`](./telegram-queue-handler) is an AWS lambda function (Rust-lang) that listens to the events in the dedicated AWS SQS queue for Telegram. Interacts with the DevConsole DB to get the `AlertRule`'s name in order to create a Telegram message
  > "Alert {name} triggered. See {link to NEAR Explorer} for details"
  Also, stores an information about the message has been sent to the Telegram.

## Design concept

Identified major types of the events on the network:

- `ACTIONS` - following the `ActionReceipts` (party of the transaction, transfer, create account, etc.)
- `EVENTS` - following the [Events Format](https://nomicon.io/Standards/EventsFormat)
- `STATE_CHANGES` *name is a subject to change* - following the `StateChanges` (account state change, stake rewards, account balances changes, etc.)

## `.env`

```
DATABASE_URL=postgres://user:pass@host/database
LAKE_AWS_ACCESS_KEY=AKI_LAKE_ACCESS...
LAKE_AWS_SECRET_ACCESS_KEY=LAKE_SECRET...
QUEUE_AWS_ACCESS_KEY=AKI_SQS_ACCESS...
QUEUE_AWS_SECRET_ACCESS_KEY=SQS_ACCESS_SECRET
QUEUE_URL=https://sqs.eu-central-1.amazonaws.com/754641474505/alertexer-queue

```
## Running locally
 * _Install postgres locally if not already present._
 * Create a local postgres database and user like so, changing the credentials to your liking:
```
psql 
CREATE DATABASE alerts;
CREATE USER alerts WITH PASSWORD 'alerts';
GRANT ALL PRIVILEGES ON DATABASE alerts TO alerts;
```
 * Update the `.env` file with the database credentials you just set. `host.docker.internal` as the hostname will point to your local host machine. 
 * Run [schema.sql](./alert-rules/schema.sql) against your alerts DB to create the alert rules tables.
 * Grant table privileges to the DB user
```
psql
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO alerts;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO alerts;
```
 * _Install docker locally if not already present._
 * Run `docker compose up`
