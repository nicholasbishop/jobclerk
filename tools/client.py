#!/usr/bin/env python3

import requests

def main():
    base_url = 'http://127.0.0.1:8000'
    resp = requests.post(base_url + '/api/projects/testproj/jobs', json={
        'hello': 'world',
    })
    print(resp.text)
    resp.raise_for_status()


if __name__ == '__main__':
    main()
