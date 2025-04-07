# Sysmon-rs
Rust driver based on https://github.com/zodiacon/windowskernelprogrammingbook/tree/master/chapter09/SysMon. The goal is to monitor system actions like:
- process creation
- thread creation
- image load
- registry set

<!-- - [Example of usage](#Example-of-usage)  -->
# Table of contents
- [Preparation](#Preparation)
- [Installation](#Installation)
- [How to use](#How-to-use)
- [Module structure](#Module-structure)
- [Latest changes](#Worth-mentioning)
- [Future plans](#Future-plans)

# Preparation
1. Install dependencies, like SDK, WDK and rust on build machine
2. You should use VM to test driver
3. You need set machine to test mode: `bcdedit.exe -set TESTSIGNING ON` and reboot

# Installation
1. Clone the repository
2. Produce cert: `cargo xtask cert`
3. Build and sign driver:  `cargo xtask driver`
4. Build client: `cargo xtask client`

# How to use
1. Install driver: `sc create sysmon type=kernel binPath=<driver.sys path>`
2. Start driver: `sc start sysmon`
3. Run client to get events: `sysmon-client.exe`
4. Finally stop driver: `sc stop sysmon`

# Module structure
- **sysmon-km** - driver project which gather particular events from system
- **sysmon-um** - user mode program to read and display events saved by driver
- **common** - shared info between driver and client, like format of data send from driver to client
- **xtask** - build system

# Latest changes
- Move from makefile.toml to xtask
- add BSD3 license

# Future plans
- add unit tests, audit and add mock tests
- github actions
- move to official sdk
- use OCSF schema to store events

# Acknowledgment
- [Driver with rust](https://not-matthias.github.io/posts/kernel-driver-with-rust/) by not-mattias
- [System monitor](https://github.com/zodiacon/windowskernelprogrammingbook/tree/bd13779bf1f79f4056d206e1f4272baf032e5451/chapter09/SysMon) by Pavel Yosifovich
