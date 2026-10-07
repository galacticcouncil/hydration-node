# gc-runner

self-hosted actions runner image behind the `lark` label. ephemeral: each replica registers, runs one job in a sibling container, deregisters.

```
docker build -t galacticcouncil/gc-runner:2.338.0-jammy -t galacticcouncil/gc-runner:latest infrastructure/gc-runner
docker push galacticcouncil/gc-runner:2.338.0-jammy && docker push galacticcouncil/gc-runner:latest
```

service env: `ORG`, `TOKEN` (pat with org runner admin), `LABELS`. bump `RUNNER_VERSION` when github raises the minimum runner version, otherwise registration fails and `lark` jobs queue forever.
