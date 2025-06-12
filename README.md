# [Grist](https://www.getgrist.com/) Image Optimizer

The Grist Image Optimizer is a CLI tool designed to reduce image attachment size in [Grist](https://www.getgrist.com/) by converting larger image files to more efficient formats (WEBP) (and by losing some detail). It uses [grist-client-rs](https://github.com/QazCetelic/grist-client-rs).

The image optimization process is particularly useful for users accessing Grist using cellular data.
Large multi-MB images can consume substantial amounts of data, and it can add up quite quickly if each row in a spreadsheet contains one.

## Options

| Flag                                            | Environment Variable       | Description                                                                                                                 |
|-------------------------------------------------|----------------------------|-----------------------------------------------------------------------------------------------------------------------------|
| `-u`, `--base-url <BASE_URL>`                   | `GIO_BASE_URL`             | Instance URL (e.g. https://grist.mydomain.net/api)                                                                          |
| `-d`, `--dir <DIR>`                             | `GIO_TEMPORARY_DIRECTORY`  | Temporary directory (e.g. /tmp/)                                                                                            |
| `-t`, `--token <TOKEN>`                         | `GIO_API_TOKEN`            | Grist user API-token                                                                                                        |
| `-m`, `--conversion-method <CONVERSION_METHOD>` | `GIO_CONVERSION_METHOD`    | Attachment conversion method [default: normal] <br> [possible values: fastest, faster, fast, normal, slow, slower, slowest] |
| `-s`, `--specific-document <SPECIFIC_DOCUMENT>` | -                          | A specific document or nothing to scan all documents                                                                        |
| `-c`, `--concurrent-downloads`                  | `GIO_CONCURRENT_DOWNLOADS` | The limit of concurrent attachment downloads                                                                                |