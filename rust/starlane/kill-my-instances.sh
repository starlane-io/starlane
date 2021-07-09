#!/bin/bash



kill -9 $( ps -u $(whoami) | grep starlane | awk '{ print $2 }' ) 2>/dev/null
