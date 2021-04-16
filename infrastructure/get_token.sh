#!/bin/bash
header1="Accept: application/vnd.github.v3+json"
header2="Authorization: token $1"
apiUrl="https://api.github.com/orgs/galacticcouncil/actions/runners/registration-token"

cmd="$(curl -X POST -H "$header1" -H "$header2" "$apiUrl" | jq -r '.token')"
echo "${cmd}"