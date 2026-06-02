FROM node:22-alpine

WORKDIR /app

COPY package.json ./
RUN npm install --omit=dev --no-audit --no-fund && npm cache clean --force

COPY check.mjs ./

ENV STATE_FILE=/app/state/.state.json \
    ALERT_COOLDOWN_HOURS=12 \
    LEADIN_ALERT_DAYS=3 \
    TOTAL_ALERT_DAYS=7 \
    CHECK_INTERVAL_SECONDS=3600

VOLUME ["/app/state"]

# Run the check, then sleep for the configured interval, forever.
CMD ["sh", "-c", "while true; do node check.mjs; sleep ${CHECK_INTERVAL_SECONDS:-3600}; done"]
