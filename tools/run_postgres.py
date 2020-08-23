#!/usr/bin/env python3

import subprocess


def run_cmd(*cmd):
    print(' '.join(cmd))
    subprocess.run(cmd, check=True)


def main():
    name = 'jobline-postgres'

    run_cmd('docker', 'run', '--rm', '--name', name,
            '--publish', '5432:5432',
            # Allow all connections without a password. This is just a
            # test database so it's fine.
            '-e', 'POSTGRES_HOST_AUTH_METHOD=trust',
            '-d', 'postgres:alpine')


if __name__ == '__main__':
    main()
