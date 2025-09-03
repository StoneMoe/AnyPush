FROM python:3.6-alpine

WORKDIR /anypush

COPY requirements.txt requirements.txt
RUN pip3 install -r requirements.txt --no-cache-dir

COPY . .
EXPOSE 587

ENTRYPOINT ["python", "/anypush/main.py"]
