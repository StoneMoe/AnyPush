#!/usr/bin/env python
# -*- coding: utf-8 -*-


class BaseError(Exception):
    """Inherit Errors from this"""

    def __init__(self, message=None, meta=None):
        self.message = message
        self.meta = meta

    def __str__(self):
        return self.message or 'No error message specified.'


class InvalidConfigError(BaseError):
    def __init__(self, config_name, message=None, meta=None):
        super(InvalidConfigError, self).__init__('%s(%s)' % (config_name, message), meta)
