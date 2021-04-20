#!/bin/bash
bash_source=$(source /home/ubuntu/config_script.sh)
echo $bash_source
./config_script.sh $ACCESS_TOKEN &