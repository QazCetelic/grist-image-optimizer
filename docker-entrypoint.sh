#!/bin/sh

interval=${GIO_INTERVAL:-300}
while true; do
  ./grist-image-optimizer
  echo "Waiting for $interval seconds..."
  sleep "$interval"
done