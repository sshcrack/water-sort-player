#!/bin/bash
DEVICE_IP="10.6.0.15"
adb connect $DEVICE_IP:$(nmap $DEVICE_IP -p 37000-44000 | awk "/\/tcp/" | cut -d/ -f1)