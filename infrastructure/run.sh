#!/bin/bash
bash_source=$(bash source /home/ubuntu/config_script.sh)
echo $bash_source
run=$(bash /home/ubuntu/config_script.sh $ACCESS_TOKEN &)
echo $run