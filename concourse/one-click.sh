#!/bin/bash
set -exuo pipefail

list_jobs() {
    fly -t ${TARGET} jobs -p ${PIPELINE}
}

list_jobs | awk '{print $1}' |
    while read JOB; do
        fly -t ${TARGET} trigger-job --job ${PIPELINE}/${JOB} --watch >${PIPELINE}_${JOB} 2>&1
    done

list_jobs
