# Releasing a new version

This should be automated, but for now—

## Update changelog

```
NEWVERSION=0.x.0
towncrier --draft --version=$NEWVERSION | less
towncrier --version=$NEWVERSION
```

## Add a tag

Annotated.

## Upload wheels

```
rm -f dist/*
make wheel
twine upload dist/*.whl
```
