/*
Copyright 2021.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package controllers

import (
	"context"
	starlanev1alpha1 "github.com/mechtronium/starlane/api/v1alpha1"
	batchv1 "k8s.io/api/batch/v1"
	"k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"strings"
)

// StarlaneProvisionerReconciler reconciles a StarlaneProvisioner object
type StarlaneProvisionerReconciler struct {
	client.Client
	Scheme *runtime.Scheme
}

//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlaneprovisioners,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlaneprovisioners/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlaneprovisioners/finalizers,verbs=update
//+kubebuilder:rbac:groups=batch,resources=jobs,verbs=get;list;watch;create;update;patch;delete

// Reconcile is part of the main kubernetes reconciliation loop which aims to
// move the current state of the cluster closer to the desired state.
// TODO(user): Modify the Reconcile function to compare the state specified by
// the StarlaneProvisioner object against the actual cluster state, and then
// perform operations to make the cluster state reflect the state specified by
// the user.
//
// For more details, check Reconcile and its Result here:
// - https://pkg.go.dev/sigs.k8s.io/controller-runtime@v0.8.3/pkg/reconcile
func (r *StarlaneProvisionerReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	log := log.FromContext(ctx)

	provisioner := &starlanev1alpha1.StarlaneProvisioner{}
	err := r.Get(ctx, req.NamespacedName, provisioner)
	if err != nil {
		if errors.IsNotFound(err) {
			// Request object not found, could have been deleted after reconcile request.
			// Owned objects are automatically garbage collected. For additional cleanup logic use finalizers.
			// Return and don't requeue
			log.Info("StarlaneProvisioner resource not found. Ignoring since object must be deleted")
			return ctrl.Result{}, nil
		}
		// Error reading the object - requeue the request.
		log.Error(err, "Failed to get StarlaneResource")
		return ctrl.Result{}, err
	}

	_, labeled := provisioner.Labels["type"]

	log.Info("Provisioning TKS", "tks", provisioner.Spec.TypeKindSpecific)
	log.Info("Provisioning labeled", "labeled", labeled)
	if !labeled {
		if provisioner.Labels == nil {
			provisioner.Labels = make(map[string]string)
		}
		tks := ParseTypeKindSpecific(provisioner.Spec.TypeKindSpecific)
		provisioner.Labels["type"] = tks.Type
		provisioner.Labels["kind"] = tks.Kind
		provisioner.Labels["vendor"] = tks.Specific.Vendor
		provisioner.Labels["product"] = tks.Specific.Product
		provisioner.Labels["variant"] = tks.Specific.Variant
		provisioner.Labels["version"] = tks.Specific.Version
		err = r.Update(ctx, provisioner)
		if err != nil {
			log.Error(err, "could not update StarlaneProvisioner")
			return ctrl.Result{}, err
		}
		log.Info("StarlaneProvisioner labels updated")
	}

	return ctrl.Result{}, nil
}

// SetupWithManager sets up the controller with the Manager.
func (r *StarlaneProvisionerReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&starlanev1alpha1.StarlaneProvisioner{}).
		Owns(&batchv1.Job{}).
		Complete(r)
}

type TypeKindSpecific struct {
	Type     string
	Kind     string
	Specific Specific
}

type Specific struct {
	Vendor  string
	Product string
	Variant string
	Version string
}

func ParseSpecific(src string) Specific {
	parts := strings.Split(src, ":")
	return Specific{
		Vendor:  parts[0],
		Product: parts[1],
		Variant: parts[2],
		Version: parts[3],
	}
}

func ParseTypeKindSpecific(src string) TypeKindSpecific {
	parts := strings.Split(src, "<")

	return TypeKindSpecific{
		Type:     parts[1],
		Kind:     parts[2],
		Specific: ParseSpecific(strings.TrimSuffix(parts[3], ">>>")),
	}
}
