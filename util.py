#!/usr/bin/env python
# -*- coding: utf-8 -*-

import os

from error import InvalidConfigError


class NotSet:
    pass


def env_conf(name, hint, type=NotSet, empty=False, default=NotSet):
    """
    Get config value by name

    :param type: callable for convert config value
    :param default: default config value
    :param name: config name
    :param hint: hint message when error raised
    :param empty: allow empty value
    :raise InvalidConfigError: when config value not meet requirements
    :return: config value
    """
    conf = os.getenv(name)
    if empty is False and not conf:
        if default is not NotSet:
            return default
        else:
            raise InvalidConfigError(name, hint)
    if type is not NotSet and callable(type):
        if type is bool:
            return smart_bool(conf)
        else:
            return type(conf)
    return conf


def smart_bool(value):
    if isinstance(value, str):
        value = value.lower().strip()
        return value not in ['false', 'no', '0', '']
    else:
        return bool(value)
