Setup Instructions:

Follow the steps below to set up and run the project locally.

1. Clone the Repository

git clone {repo-url}

cd {repo-directory}/crates/open-router

-------------------------------------------------------------------------------------------------------------------------

2. Install Docker

Make sure Docker is installed on your system.
You can download and install Docker Desktop from the below links.

Mac - https://docs.docker.com/desktop/setup/install/mac-install/

Windows - https://docs.docker.com/desktop/setup/install/windows-install/

Linux - https://docs.docker.com/desktop/setup/install/linux/

-------------------------------------------------------------------------------------------------------------------------

3. Run the Project

a. First-Time Setup

If you're setting up the environment for the first time, run:

make init

This command performs the following under the hood:

docker-compose run --rm db-migrator && docker-compose up open-router

This will:

.Set up the environment

.Set up the database with the required schema

.Sets up redis and the server for running the application

.Push the configs defined in the config.yaml & the static rules defined for routing in priority_logic.txt to the DB


b. Start the Server (without resetting DB)

If the DB schema is already set up and you don’t want to reset the DB, use:

make run


c. Update Configs / Static Rules

To update the configs (from the config.yaml file) or the static rules (from priority_logic.txt), run:

make update-config


d. Stop Running Instances

To stop the running Docker instances:

make stop

-------------------------------------------------------------------------------------------------------------------------

4. Running Local Code Changes

If you've made changes to the code locally and want to test them:

a. Initialize Local Environment

make init-local

This command performs the following under the hood:

docker-compose run --rm db-migrator && docker-compose up open-router-local

b. Run Locally

make run-local
