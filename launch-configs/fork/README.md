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
