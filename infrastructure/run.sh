#!/bin/bash
bash source /home/ubuntu/config_script.sh
run=$(bash /home/ubuntu/config_script.sh $ACCESS_TOKEN &)
echo $run