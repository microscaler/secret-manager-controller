-- Create functions to get unique filter values for GCP secrets and parameters

-- Get unique environments from GCP secrets
CREATE OR REPLACE FUNCTION gcp.get_secret_environments()
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.environment
    FROM gcp.secrets s
    WHERE s.environment IS NOT NULL
    ORDER BY s.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from GCP secrets
CREATE OR REPLACE FUNCTION gcp.get_secret_locations()
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.location
    FROM gcp.secrets s
    WHERE s.location IS NOT NULL
    ORDER BY s.location;
END;
$$ LANGUAGE plpgsql;

-- Get unique project IDs from GCP secrets (extracted from key format)
CREATE OR REPLACE FUNCTION gcp.get_secret_projects()
RETURNS TABLE(project_id TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT 
        (regexp_match(s.key, '^projects/([^/]+)/secrets/'))[1] AS project_id
    FROM gcp.secrets s
    WHERE s.key ~ '^projects/[^/]+/secrets/'
    ORDER BY project_id;
END;
$$ LANGUAGE plpgsql;

-- Get unique environments from GCP parameters
CREATE OR REPLACE FUNCTION gcp.get_parameter_environments()
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT p.environment
    FROM gcp.parameters p
    WHERE p.environment IS NOT NULL
    ORDER BY p.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from GCP parameters
CREATE OR REPLACE FUNCTION gcp.get_parameter_locations()
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT p.location
    FROM gcp.parameters p
    WHERE p.location IS NOT NULL
    ORDER BY p.location;
END;
$$ LANGUAGE plpgsql;

