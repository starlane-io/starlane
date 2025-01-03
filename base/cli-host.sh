#!/bin/bash


export command="$1"

case "$command" in
    "read")
        echo "Hello World" ;;

    "write")
        cat - ;;
esac