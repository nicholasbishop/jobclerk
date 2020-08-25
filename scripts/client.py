#!/usr/bin/env python3

import argparse
import pprint
import requests

def main():
    add_job = 'add-job'
    get_jobs = 'get-jobs'
    take_job = 'take-job'

    parser = argparse.ArgumentParser()
    parser.add_argument('cmd', choices=(add_job, get_jobs, take_job))
    args = parser.parse_args()

    base_url = 'http://127.0.0.1:8000'

    if args.cmd == get_jobs:
        resp = requests.get(base_url + '/api/projects/testproj/jobs')
    elif args.cmd == add_job:
        resp = requests.post(base_url + '/api/projects/testproj/jobs', json={
            'hello': 'world',
        })
    elif args.cmd == take_job:
        resp = requests.post(base_url + '/api/projects/testproj/take-job', json={
            'runner': 'testrunner'
        })

    # TODO: return API errors as JSON too so that this isn't needed
    try:
        resp.raise_for_status()
        pprint.pprint(resp.json())
    except:
        print(resp.text)
        raise


if __name__ == '__main__':
    main()
