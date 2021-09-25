#!/bin/bash

starlane create "localhost<Space>"
starlane create "localhost:my-files<FileSystem>"
starlane cp websites/simple-site1/index.html "localhost:my-files:/index.html"
starlane publish ./reverse-proxy-config "localhost:config:1.0.0"
starlane set "localhost::config=localhost:config:1.0.0:/forward-to-filesystem.conf"

