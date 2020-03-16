#!/usr/bin/env python
# -*- coding: utf-8 -*-
import asyncore
import email
import logging
import smtpd
import socket
import sys
from email.header import decode_header
from email.message import Message

import requests

from util import env_conf

logging.basicConfig(level=logging.INFO, stream=sys.stdout)
logger = logging.getLogger('AnyPush')
webhook_url = env_conf('WEBHOOK_URL', 'SMTP to Webhook', str)
default_charset = 'ASCII'


class CustomSMTPServer(smtpd.SMTPServer):

    def process_message(self, peer: tuple, mailfrom: str, rcpttos: list, data: bytes, **kwargs):
        msg: Message = email.message_from_bytes(data)
        decoded_header: list = decode_header(msg.get('Subject'))
        subject = ''.join([
            str(item[0], encoding=item[1] or default_charset) if isinstance(item[0], bytes) else item[0]
            for item in decoded_header
        ])
        data = {
            'from': mailfrom,
            'to': rcpttos.pop(),
            'subject': subject,
            'text': msg.get_payload()
        }
        logger.info('Redirect Email "%s" to "%s"' % (subject, data['to']))
        try:
            requests.post(webhook_url, json=data)
        except Exception as e:
            logger.error('Webhook request failed: %r' % e)


if __name__ == '__main__':
    host_name = socket.gethostname()
    host_ip = socket.gethostbyname(host_name)
    server = CustomSMTPServer(('0.0.0.0', 1025), None)
    logger.info("Hostname: %s" % host_name)
    logger.info("IP: %s" % host_ip)
    logger.info("Listening: %s" % str(server._localaddr))
    asyncore.loop()
