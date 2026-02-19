-- Initialize databases for HyperSpot modules
-- This script runs automatically when MariaDB container starts
-- Each module gets its own database and dedicated user for isolation

-- Create module-specific users first
CREATE USER IF NOT EXISTS 'settings_user'@'%' IDENTIFIED BY 'settings_pass';
CREATE USER IF NOT EXISTS 'users_info_user'@'%' IDENTIFIED BY 'users_info_pass';

-- Create databases with proper character set and collation
CREATE DATABASE IF NOT EXISTS settings_db CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE DATABASE IF NOT EXISTS users_info_db CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Grant permissions: each user only has access to their own database
GRANT ALL PRIVILEGES ON settings_db.* TO 'settings_user'@'%';
GRANT ALL PRIVILEGES ON users_info_db.* TO 'users_info_user'@'%';

-- Flush privileges to ensure changes take effect
FLUSH PRIVILEGES;
