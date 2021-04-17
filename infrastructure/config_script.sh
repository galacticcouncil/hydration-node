curl -o actions-runner-linux-x64-2.277.1.tar.gz -L https://github.com/actions/runner/releases/download/v2.277.1/actions-runner-linux-x64-2.277.1.tar.gz

tar xzf ./actions-runner-linux-x64-2.277.1.tar.gz
TOKEN=$(bash get_token.sh $1)
echo $TOKEN
bash config.sh --url https://github.com/galacticcouncil/HydraDX-node --token $TOKEN --name "aws-runner" --work _work --labels aws

bash ./run.sh