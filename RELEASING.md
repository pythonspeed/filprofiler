# Releasing a new version

This should be automated, but for now—

## Update changelog

```
NEWVERSION=0.x.0
towncrier --draft --version=$NEWVERSION | less
towncrier --version=$NEWVERSION
```

## Commit the code, push to master

## Add a tag

Annotated.

GitHub Actions should then build wheels and upload them.
