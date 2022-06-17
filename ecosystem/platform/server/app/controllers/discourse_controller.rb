# frozen_string_literal: true

# Copyright (c) Aptos
# SPDX-License-Identifier: Apache-2.0

require 'discourse_api'

class DiscourseController < ApplicationController
  before_action :authenticate_user!
  before_action :ensure_confirmed!

  def sso
    secret = ENV.fetch('DISCOURSE_SECRET', 'MY_SECRET_STRING')
    sso = DiscourseApi::SingleSignOn.parse(request.query_string, secret)

    sso.email = current_user.email
    sso.username = current_user.username || current_user.authorizations.pluck(:username).first
    sso.external_id = current_user.external_id # unique id for each user of your application
    sso.sso_secret = secret

    redirect_to sso.to_url('https://forum.aptoslabs.com/session/sso_login'), allow_other_host: true
  end
end
