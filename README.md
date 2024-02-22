SysMon

sysmon-driver-rust - rust driver based on https://github.com/zodiacon/windowskernelprogrammingbook/tree/master/chapter09/SysMon

Installing (with admin rights):
> sc create sysmon type=kernel binPath=<driver.sys path>

Start: 
> sc start sysmon

Stop:
> sc stop sysmon