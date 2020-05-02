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
from typing import Union, Optional, List

import requests

from util import env_conf

logging.basicConfig(level=logging.INFO, stream=sys.stdout)
logger = logging.getLogger('AnyPush')
webhook_url = env_conf('WEBHOOK_URL', 'SMTP to Webhook', str)
default_charset = 'ASCII'


class Mail:
    def __init__(self, peer: tuple, mailfrom: str, rcpttos: Optional[List[str]],
                 data: bytes, **kwargs):
        self.client_ip = peer[0]
        self.client_port = peer[1]
        self.mail_from: str = mailfrom
        self.rcpt_to_list: List[str] = rcpttos
        self.raw_data: bytes = data
        self.mail_options: List[str] = kwargs.get('mail_options')
        self.rcpt_options: List[str] = kwargs.get('rcpt_options')

        self.message: Message = email.message_from_bytes(self.raw_data)

    @property
    def subject(self) -> str:
        decoded_header: list = decode_header(self.message.get('Subject'))
        subject = ''.join([
            str(item[0], encoding=item[1] or default_charset) if isinstance(item[0], bytes) else item[0]
            for item in decoded_header
        ])
        return subject

    @property
    def sender(self) -> str:
        """the raw address the client claims the message is coming from"""
        return self.mail_from

    @property
    def to(self) -> List[str]:
        """a list of raw addresses the client wishes to deliver the message to"""
        return self.rcpt_to_list

    @property
    def payload(self) -> Union[str, List[Message]]:
        return self.message.get_payload()

    @property
    def text(self) -> str:
        return self._text_from_msg(self.message)

    def _text_from_msg(self, message: Message) -> str:
        _payload = message.get_payload()
        if isinstance(_payload, str):
            return _payload
        elif isinstance(_payload, list):
            return '\n'.join([self._text_from_msg(item) for item in self.payload])
        else:
            raise NotImplementedError('expect str or List[Message] payload, but it was %s' % type(self.payload))


class CustomSMTPServer(smtpd.SMTPServer):

    def process_message(self, peer: tuple, mailfrom: str, rcpttos: list, data: bytes, **kwargs):
        """
        SMTP message processing

        docstring are copied from smtpd.SMTPServer

        :param peer: a tuple containing (ipaddr, port) of the client that made the socket connection to our smtp port.
        :param mailfrom: the raw address the client claims the message is coming from.
        :param rcpttos: a list of raw addresses the client wishes to deliver the message to.
        :param data: a string containing the entire full text of the message,
                     headers (if supplied) and all.  It has been `de-transparencied'
                     according to RFC 821, Section 4.5.2.  In other words, a line
                     containing a `.' followed by other text has had the leading dot
                     removed.
        :param kwargs: a dictionary containing additional information.  It is
                       empty if decode_data=True was given as init parameter, otherwise
                       it will contain the following keys:
                       'mail_options': list of parameters to the mail command.  All
                                       elements are uppercase strings.  Example:
                                       ['BODY=8BITMIME', 'SMTPUTF8'].
                       'rcpt_options': same, for the rcpt command.

        :return: None for a normal `250 Ok' response;
                 otherwise, it should return the desired response string in RFC 821 format.
        """
        msg = Mail(peer, mailfrom, rcpttos, data, **kwargs)
        # Parse data

        data = {
            'from': msg.sender,
            'to': ','.join(msg.to),
            'subject': msg.subject,
            'text': msg.text
        }
        logger.info('Redirect Email "%s" to "%s"' % (msg.subject, msg.to))
        try:
            requests.post(webhook_url, json=data)
        except Exception as e:
            logger.error('Webhook request failed: %r' % e)


if __name__ == '__main__':
    host_name = socket.gethostname()
    host_ip = socket.gethostbyname(host_name)
    listen_at = ('0.0.0.0', 587)
    server = CustomSMTPServer(listen_at, None)
    logger.info("Hostname: %s" % host_name)
    logger.info("IP: %s" % host_ip)
    logger.info("Listening: %s" % str(listen_at))
    asyncore.loop()
