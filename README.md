# [Grist](https://www.getgrist.com/) Image Optimizer

***⚠️ WIP ⚠️**: This tool is currently a work in progress and might have unintended side effects.*

The Grist Image Optimizer is a CLI tool designed to reduce image attachment size in [Grist](https://www.getgrist.com/) by converting larger image files to more efficient formats (WEBP) (and by losing some detail). It uses [grist-client-rs](https://github.com/QazCetelic/grist-client-rs).

The image optimization process is particularly useful for users accessing Grist using cellular data.
Large multi-MB images can consume substantial amounts of data, and it can add up quite quickly if each row in a spreadsheet contains one.
However, note that it will have a limited effect on the file size of the document due to an issue that prevents the files from actually being removed (https://github.com/gristlabs/grist-core/issues/1573) and might actually increase overal file size.

## Options

| Flag                         | Description                                                                 |
|------------------------------|-----------------------------------------------------------------------------|
| `-u, --base-url <BASE_URL>` | Instance URL (e.g. https://grist.mydomain.net/api)                       |
| `-d, --dir <DIR>`           | Temporary directory (e.g. /tmp/)                                          |
| `-t, --token <TOKEN>`       | Grist user API-token                                                            |
| `-c, --conversion-method <CONVERSION_METHOD>` | Attachment conversion method [default: normal] <br> [possible values: fastest, faster, fast, normal, slow, slower, slowest] |
| `-s, --specific-document <SPECIFIC_DOCUMENT>` | A specific document or nothing to scan all documents |
