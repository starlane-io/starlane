#!/bin/bash

# this little bash script kills the process that is holding port 4343
# for some reason rust does not just die when intellij attempts to kill it


set -x

kill -9 $(lsof -i:4343 -t)
