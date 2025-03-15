starts zombienet instance with forked state downloaded from `STATE_SOURCE` (by default loaded from latest snapshot available)

### run with docker
```
docker run -d -p 9988:9988 galacticcouncil/fork
```

### run locally
- node >18 required
- you have to have all binaries present on correct paths in `config.json`
```
npm i && npm start
```

### test accounts

besides the regular substrate test account `//Alice` there is also test evm wallet
which has privilege to deploy contracts on chain:

```
Private key: 42d8d953e4f9246093a33e9ca6daa078501012f784adfe4bbed57918ff13be14
Address:     0x222222ff7Be76052e023Ec1a306fCca8F9659D80
Account Id:  45544800222222ff7be76052e023ec1a306fcca8f9659d800000000000000000
SS58(63):    7KATdGakyhfBGnAt3XVgXTL7cYjzRXeSZHezKNtENcbwWibb
SS58(42):    5DdcCSDHubHuzYg92M2BbXkC3MjpGgRWbY2EQ2Nuef7hbxwp
```
