# Google Sheets API Example

## Requirements

- [Google service account](https://developers.google.com/identity/protocols/oauth2/service-account#creatinganaccount)
  - See [`example.env`](../../example.env) for an example of what your `.env` file should look like.
    Please place it in the root of the workspace. (Next to `example.env`)
    <!-- TODO(c-git): Change to .env file per project -->
    <!-- TODO(c-git): Explain how to fill in the values -->

## What it does

- Logs in using OAuth
- Single Cell Read/Write
  - Reads cell `A1` on "Sheet1" (or creates if not present)
  - Increments the value if it's a number
  - Write the incremented value or 1 if it wasn't an umber

## TODOs

- [ ] Add delete of a sheet
- [ ] Add append
- [ ] Add saving temporary token for reuse
- [ ] Full Sheet Read/Write
  - Reads ALL values from the tab named "Sheet2"
  - Updates some of the values (We need to make it more clear what updates will be done)
  - Overwrites all the values
