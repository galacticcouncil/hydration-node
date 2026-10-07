#!/bin/sh

./config.sh --url https://github.com/"$ORG" --pat "$TOKEN" --name "$NAME" --unattended --ephemeral --labels "$LABELS"
CONTAINER=$(sudo docker create galacticcouncil/gc-runner:latest ./run.sh)
sudo docker cp .credentials_rsaparams $CONTAINER:/home/runner
sudo docker cp .credentials $CONTAINER:/home/runner
sudo docker cp .env $CONTAINER:/home/runner
sudo docker cp .path $CONTAINER:/home/runner
sudo docker cp .runner $CONTAINER:/home/runner
sudo docker start -a $CONTAINER
sudo docker rm -f $CONTAINER
./config.sh remove --pat "$TOKEN"