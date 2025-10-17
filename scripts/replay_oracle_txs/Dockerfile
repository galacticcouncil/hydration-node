FROM python:3.12.11
WORKDIR /usr/app/src

COPY oracles.py requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

ENTRYPOINT [ "python", "./oracles.py"]