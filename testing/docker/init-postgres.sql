-- Initialize databases for HyperSpot modules
-- This script runs automatically when PostgreSQL container starts
-- Each module gets its own database and dedicated user for isolation

-- Create module-specific users first
CREATE USER settings_user WITH PASSWORD 'settings_pass';
CREATE USER users_info_user WITH PASSWORD 'users_info_pass';

-- Create databases with the module users as owners
CREATE DATABASE settings_db OWNER settings_user;
CREATE DATABASE users_info_db OWNER users_info_user;

-- Connect to settings_db and create dedicated schema
\c settings_db;
CREATE SCHEMA settings AUTHORIZATION settings_user;
GRANT ALL PRIVILEGES ON SCHEMA settings TO settings_user;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA settings TO settings_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA settings TO settings_user;
ALTER DEFAULT PRIVILEGES FOR USER settings_user IN SCHEMA settings GRANT ALL PRIVILEGES ON TABLES TO settings_user;
ALTER DEFAULT PRIVILEGES FOR USER settings_user IN SCHEMA settings GRANT ALL PRIVILEGES ON SEQUENCES TO settings_user;
-- Set search_path so the schema is used by default
ALTER DATABASE settings_db SET search_path TO settings;

-- Connect to users_info_db and create dedicated schema
\c users_info_db;
CREATE SCHEMA users_info AUTHORIZATION users_info_user;
GRANT ALL PRIVILEGES ON SCHEMA users_info TO users_info_user;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA users_info TO users_info_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA users_info TO users_info_user;
ALTER DEFAULT PRIVILEGES FOR USER users_info_user IN SCHEMA users_info GRANT ALL PRIVILEGES ON TABLES TO users_info_user;
ALTER DEFAULT PRIVILEGES FOR USER users_info_user IN SCHEMA users_info GRANT ALL PRIVILEGES ON SEQUENCES TO users_info_user;
-- Set search_path so the schema is used by default
ALTER DATABASE users_info_db SET search_path TO users_info;
