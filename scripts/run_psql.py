#!/usr/bin/env python3

import subprocess


def run_cmd(*cmd):
    print(' '.join(cmd))
    subprocess.run(cmd, check=True)

def main():
    name = 'jobclerk-postgres'

    run_cmd('docker', 'run',
            '--rm',
            '--network', 'host',
            '-it',
            'postgres:alpine',
            'psql',
            '-h', 'localhost',
            '-U', 'postgres')


if __name__ == '__main__':
    main()
