FROM docker:29.1.3-dind-rootless

# Nested image extraction can remap the uidmap helpers to the rootless user.
# Restore the ownership and privilege bits required by rootlesskit.
USER root
RUN chown root:root /usr/bin/newuidmap /usr/bin/newgidmap \
    && chmod u+s /usr/bin/newuidmap /usr/bin/newgidmap
USER rootless
