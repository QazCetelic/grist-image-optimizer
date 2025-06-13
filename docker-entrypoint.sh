#!/bin/sh

if [ -z "$GIO_BASE_URL" ]; then
  echo "Error: GIO_BASE_URL is not set."
  exit 1
fi

if [ -z "$GIO_TEMPORARY_DIRECTORY" ]; then
  echo "Error: GIO_TEMPORARY_DIRECTORY is not set."
  exit 1
fi

if [ -z "$GIO_API_TOKEN" ]; then
  echo "Error: GIO_API_TOKEN is not set."
  exit 1
fi

interval=${GIO_INTERVAL:-300}
tmp_dir=${GIO_TEMPORARY_DIRECTORY:-"/tmp/images"}

while true; do
  ./grist-image-optimizer --dir "$tmp_dir"
  echo "Waiting for $interval seconds..."
  sleep "$interval"
done