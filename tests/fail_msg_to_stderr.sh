#!/usr/bin/env bash

# echo msg to stderr and exit with failure
fail_msg_to_stderr() {
    echo -n "$1" 1>&2
    exit 2
}

fail_msg_to_stderr "This is a failure message"
