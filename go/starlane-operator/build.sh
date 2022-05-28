#!/bin/bash

make docker-build docker-push IMG="starlane/starlane-operator:snapshot" DEFAULT_STARLANE="starlane/starlane:snapshot"
