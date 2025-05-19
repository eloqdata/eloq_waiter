#!/bin/bash
# Shared utility functions for test scripts

# Helper: wait for cluster to be ready
wait_for_cluster_ready() {
    set +ex
    local MAX_RETRIES=30
    local RETRY_DELAY=2
    local COUNT=0
    local TARGET="${1:-${TX_IP}:${LEADER_PORT}}"

    local IP=$(echo $TARGET | cut -d':' -f1)
    local PORT=$(echo $TARGET | cut -d':' -f2)
    local CLI="redis-cli -h ${IP} -p ${PORT}"

    while [ $COUNT -lt $MAX_RETRIES ]; do
        local OUTPUT=$($CLI get k 2>&1)
        if [[ $? -eq 0 ]]; then
            set -ex
            echo "Cluster is ready (verified with ${TARGET})."
            return 0
        elif [[ "$OUTPUT" == *"Failed to initialize the transaction"* ]]; then
            echo "Transaction initialization failed on ${TARGET}, retrying..."
            local INIT_RETRIES=10
            local INIT_COUNT=0
            while [ $INIT_COUNT -lt $INIT_RETRIES ]; do
                sleep 1
                OUTPUT=$($CLI get k 2>&1)
                if [[ $? -eq 0 ]] || [[ "$OUTPUT" != *"Failed to initialize the transaction"* ]]; then
                    set -ex
                    echo "Cluster is ready after transaction retry (verified with ${TARGET})."
                    return 0
                fi
                ((INIT_COUNT++))
                echo "Transaction initialization retry $INIT_COUNT/$INIT_RETRIES on ${TARGET}..."
            done
        else
            echo "Waiting for cluster to be ready on ${TARGET}..."
            sleep $RETRY_DELAY
            ((COUNT++))
        fi
    done

    set -ex
    echo "Cluster is not ready on ${TARGET} after $MAX_RETRIES retries."
    exit 1
}

# Helper: run a command and validate result
run_command() {
    local CMD="$1"
    local EXPECTED_RESULT="$2"
    local OUTPUT=""
    local RETRIES=20
    local ATTEMPT=0
    local STATUS=0

    echo "Starting command: $CMD with expected result: $EXPECTED_RESULT"

    while [ $ATTEMPT -lt $RETRIES ]; do
        echo "Running attempt $((ATTEMPT+1))/$RETRIES..."
        set +ex
        OUTPUT=$(eval "$CMD" 2>&1)
        STATUS=$?
        set -ex
        echo "Attempt $((ATTEMPT+1)) result: Status $STATUS, Output: '$OUTPUT'"
        if [[ $STATUS -eq 0 && "$OUTPUT" == "$EXPECTED_RESULT" ]]; then
            echo "Success: Output matches expected result"
            return 0
        elif [[ "$OUTPUT" != "$EXPECTED_RESULT" ]]; then
            echo "Transaction initialization failed, retrying ($((ATTEMPT+1))/$RETRIES)..."
            sleep 2
            ((++ATTEMPT))
        else
            echo "Not a transaction error, checking result normally"
            break
        fi
    done

    if [[ $STATUS -ne 0 ]]; then
        echo "Error executing command: $CMD"
        echo "Output: $OUTPUT"
        exit 1
    fi

    if [[ "$OUTPUT" != "$EXPECTED_RESULT" ]]; then
        echo "Output does not match expected result."
        echo "Expected: $EXPECTED_RESULT"
        echo "Actual: $OUTPUT"
        exit 1
    fi

    echo "Command successful after handling"
    return 0
} 