#!/bin/sh

# swarm stops the task with TERM; the sibling must not outlive it
cleanup() {
  [ -n "$CONTAINER" ] && sudo docker rm -f "$CONTAINER" >/dev/null
  ./config.sh remove --pat "$TOKEN"
}
trap 'cleanup; exit 143' TERM INT

./config.sh --url https://github.com/"$ORG" --pat "$TOKEN" --name "$NAME" --unattended --ephemeral --labels "$LABELS" || exit 1
CONTAINER=$(sudo docker create galacticcouncil/gc-runner:latest ./run.sh)
sudo docker cp .credentials_rsaparams $CONTAINER:/home/runner
sudo docker cp .credentials $CONTAINER:/home/runner
sudo docker cp .env $CONTAINER:/home/runner
sudo docker cp .path $CONTAINER:/home/runner
sudo docker cp .runner $CONTAINER:/home/runner
# backgrounded so the trap can fire while waiting
sudo docker start -a $CONTAINER &
wait $!
trap - TERM INT
cleanup
