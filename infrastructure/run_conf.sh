#!/bin/bash
bash_source=$(source /home/ubuntu/config_script.sh)
echo $bash_source
bashrun=$(bash -c "config_script.sh $ACCESS_TOKEN &")
echo $bashrun