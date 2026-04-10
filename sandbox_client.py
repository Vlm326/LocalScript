import requests

url = "http://localhost:3000/pipeline"

payload = {
    "code": "print('hello')",
    "execute": True,
    "timeout": 5000
}

response = requests.post(url, json=payload)
print(response.json())